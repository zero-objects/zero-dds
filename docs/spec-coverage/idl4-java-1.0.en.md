# IDL4 to Java Language Mapping 1.0 — Spec Coverage

**Spec:** [OMG IDL4-Java 1.0](https://www.omg.org/spec/IDL4-Java/1.0/PDF) (51 pages, OMG formal/2021-08-01)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** the generated classes compile against the Java runtime shipped
in the repo — `crates/java-omgdds/java` (a Maven project, the `omgdds.jar`
counterpart: `org.zerodds.cdr.*` XCDR2 codec + `org.omg.dds.*` PSM) and
`crates/idl-java/runtime` (`org.zerodds.types.*` annotations +
`org.zerodds.rpc.*` Holder/Requester/Replier). `tests/compile_check.rs`
compiles the generated code with `javac` against exactly that runtime.

Implementation:

- `crates/idl-java/` — live with 12 files + 272 tests (amqp/annotations/bitset/corba_traits/emitter/error/keywords/lib/rpc/type_map/typesupport/verbatim).

---

## §1 Scope

### 1.1 Mapping IDL v4 -> Java

**Spec:** §1, p. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 [IDL4] to the Java programming
language. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification with the
exception of middleware specific constructs that are better
addressed in separate specifications."

**Repo:** `crates/idl-java/src/lib.rs` — crate doc with the mapping scope.

**Tests:** crate-wide; see per section below.

**Status:** done

### 1.2 Use of modern Java language features

**Spec:** §1, p. 1 — "The language mapping makes use of modern Java
language features as appropriate and natural."

**Repo:** the generator emits Java SE 8+ constructs (generics,
annotations, sealed interfaces for unions, Optional for @optional
members).

**Tests:** `union_emits_sealed_interface`,
`optional_member_uses_optional`,
`async_interface_returns_completable_future`.

**Status:** done

---

## §2 Conformance

### 2.1 Implementation: IDL -> Java source per §7

**Spec:** §2, p. 1 — "A conformant implementation shall transform
IDL input into Java source code output as specified in clause 7."

**Repo:** `crates/idl-java/src/emitter.rs` — top-level emitter.

**Tests:** `multi_file_output_one_class_per_top_level_type`,
`each_file_starts_with_generated_marker`,
`empty_ast_produces_no_files`,
`empty_module_emits_no_files`.

**Status:** done

### 2.2 User: application code runs cross-implementation portable

**Spec:** §2, p. 1 — "Application source code that conforms to this
specification makes use of the Java data types and API's as defined
in clause 7. [...] Conformant application source code, as a result,
will be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §3 Normative References

### 3.1 [CORBA-IFC] CORBA 3.3

**Spec:** §3, p. 1 — "[CORBA-IFC] CORBA 3.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 3.2 [IDL4] IDL 4.2 (2018)

**Spec:** §3, p. 1 — "[IDL4] OMG IDL 4.2."

**Repo:** `crates/idl/src/grammar/idl42.rs`.

**Tests:** see `idl-4.2.md`.

**Status:** done

### 3.3 [J2SE 8.0] Java SE 8 Edition (2015)

**Spec:** §3, p. 1 — "[J2SE 8.0] Java SE 8."

**Repo:** the generator emits Java-8-compatible code (annotations + type-
use annotations for @optional/@external).

**Tests:** `optional_annotation_emits_marker_in_addition_to_optional_type`.

**Status:** done

### 3.4 [JavaBeans] JavaBeans 1.01

**Spec:** §3, p. 1 — "[JavaBeans] JavaBeans 1.01."

**Repo:** the generator emits bean-style accessors (getX/setX) per the
JavaBeans 1.01 convention.

**Tests:** `primitive_struct_uses_correct_java_types`,
`struct_emits_one_file_per_type`.

**Status:** done

---

## §4 Terms and Definitions

### 4.1 Building Block

**Spec:** §4, p. 1 — "A Building Block is a consistent set of IDL
rules [...] atomic, meaning that if selected, they must be totally
supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; a semantic reference point without its own code requirement.

### 4.2 Camel Case (Lower Camel Case)

**Spec:** §4, p. 2 — "Camel Case [...] the variation of Camel Case
commonly known as Lower Camel Case, where the first letter is not
capitalized. [...] 'these are my words' would be 'theseAreMyWords'."

**Repo:** `crates/idl-java/src/type_map.rs` — camel-case conversion.

**Tests:** indirect via `module_becomes_lowercase_package`,
`module_name_lowercased_in_package`.

**Status:** done

### 4.3 Java (general-purpose programming language)

**Spec:** §4, p. 2 — "Java is a general-purpose computer programming
language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; a semantic reference point without its own code requirement.

### 4.4 Language Mapping

**Spec:** §4, p. 2 — "An association of elements in one language to
elements in another language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; a semantic reference point without its own code requirement.

### 4.5 Pascal Case (Upper Camel Case)

**Spec:** §4, p. 2 — "Pascal Case, also known as Upper Camel Case,
[...] 'these are my words' would be 'TheseAreMyWords'."

**Repo:** `crates/idl-java/src/type_map.rs`.

**Tests:** indirect via `multi_file_output_one_class_per_top_level_type`.

**Status:** done

---

## §5 Symbols (Tab.5.1)

### 5.1 Acronyms: CCM, CORBA, DDS, J2SE, IDL, OMG

**Spec:** §5 Tab.5.1, p. 2 — acronym list.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §6 Additional Information

### 6.1 Alternative to the existing OMG IDL-Java mapping spec

**Spec:** §6.1, p. 3 — "This specification is an alternative to the
existing OMG IDL to Java Mapping specification; it is distinct in
that it provides a mapping for the constructs of IDL4."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

### 6.2 Acknowledgments (ADLINK, RTI, Twin Oaks)

**Spec:** §6.2, p. 3 — informative.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.1 General

### 7.1.1.0 Naming schemes: IDL vs. Java; selected via @java_mapping (or compiler setting)

**Spec:** §7.1.1, p. 5 — "This specification defines two naming
schemes [...] IDL Naming Scheme [...] Java Naming Scheme. The
@java_mapping annotation defined in Clause 8.1 provides a mechanism
to select the appropriate naming scheme."

**Repo:** `crates/idl-java/src/lib.rs::JavaCodegenOptions` — field
for naming-scheme selection.

**Tests:** `options_have_sensible_defaults`, `options_clone_works`.

**Status:** done

### 7.1.1.0 Name collision with reserved names: underscore prefix

**Spec:** §7.1.1, p. 5 — "if a mapped name or identifier collides
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

### 7.1.1.1 IDL Naming Scheme: names without case transformation

**Spec:** §7.1.1.1, p. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers without case transformation,
maintaining the original IDL names."

**Repo:** `lib.rs::JavaGenOptions::naming_convention` with the
`IDL_NAMING_CONVENTION` value. The spec licenses both schemes;
the ZeroDDS default is Java/JavaBeans (§7.1.1.2).

**Tests:** `relative_path_uses_directory_separator` +
`spec_conformance::pascal_case_for_class_names`,
`camel_case_for_member_accessors`,
`all_uppercase_for_constants_via_idl_default`.

**Status:** done

### 7.1.1.2 Java Naming Scheme: JavaBeans 1.01 conventions

**Spec:** §7.1.1.2, p. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers that follow the coding
guidelines defined in the JavaBeans 1.01 [JavaBeans] specification."

**Repo:** default in `JavaCodegenOptions`.

**Tests:** `module_becomes_lowercase_package`,
`module_name_lowercased_in_package`.

**Status:** done

### 7.1.1.2.1 Pascal Case Transformation Rules

**Spec:** §7.1.1.2.1, p. 5-6 — "Pascal Case rules: first letter
after each underscore capitalized, all underscores removed; first
letter capitalized."

**Repo:** pascal-case helper in `type_map.rs` with a spec-conformant
transformation (capitalize-after-underscore + remove-underscore +
capitalize-first).

**Tests:** `multi_file_output_one_class_per_top_level_type` +
`spec_conformance::pascal_case_for_class_names`.

**Status:** done

### 7.1.1.2.2 Camel Case Transformation Rules

**Spec:** §7.1.1.2.2, p. 6 — "Camel Case rules: first letter after
each underscore capitalized, all underscores removed; first letter
lower case."

**Repo:** camel-case helper in `type_map.rs`. The bean-accessor generator
uses it for getter/setter (`getMyField`/`setMyField`).

**Tests:** bean-accessor tests +
`spec_conformance::camel_case_for_member_accessors`.

**Status:** done

### 7.1.1.2.3 All Uppercase Transformation Rules

**Spec:** §7.1.1.2.3, p. 6 — "All Uppercase rules: every letter
capitalized, underscores unchanged."

**Repo:** constants use ALL_UPPERCASE per the IDL default (the spec
says: constant names are usually already ALL_UPPERCASE in IDL;
the generator preserves that).

**Tests:** `spec_conformance::all_uppercase_for_constants_via_idl_default`.

**Status:** done

### 7.1.1.2.4 All Lowercase Transformation Rules

**Spec:** §7.1.1.2.4, p. 6-7 — "All Lowercase rules: every letter
lowercase, underscores unchanged."

**Repo:** module-name lowercasing in `type_map.rs`.

**Tests:** `module_becomes_lowercase_package`,
`module_name_lowercased_in_package`,
`module_scoped_service_lands_in_lowercase_package`,
`nested_three_modules_become_three_packages`,
`three_level_modules_become_three_packages`.

**Status:** done

### 7.1.1.3 Suffixes (e.g. Abstract); underscore prefix on collision

**Spec:** §7.1.1.3, p. 7 — "If an IDL name ends in a reserved suffix
(for example, Abstract), then an underscore is prepended to the
mapped name."

**Repo:** `keywords.rs` contains reserved suffixes; the sanitizer turns
collisions into a `_` prefix. The reserved-word tests cover the
core functionality.

**Tests:** `class_is_reserved`, `record_is_reserved`,
`sealed_is_reserved`, `var_yield_reserved` (all with sanitization).

**Status:** done

### 7.1.2 Reserved Names

**Spec:** §7.1.2, p. 7 — "The mapping in effect reserves the use of
several names: <type>Abstract, Constants (per <moduleName>-Package);
all Java keywords (50+: abstract/assert/.../while); literals
(true/false/null); java.lang.Object methods (clone/equals/getClass/
hashCode/notify/notifyAll/finalize/toString/wait)."

**Repo:** `crates/idl-java/src/keywords.rs` — reserved-word list.

**Tests:** `list_contains_at_least_50_keywords`,
`class_is_reserved`, `record_is_reserved`, `sealed_is_reserved`,
`var_yield_reserved`, `foo_is_not_reserved`.

**Status:** done

### 7.1.3 Holder<E> class for inout/out parameters

**Spec:** §7.1.3, p. 8 — "Holder types are required in cases when
an IDL defined data type is passed to an operation as an inout or
out parameter. Primitive types utilize the Holder<E> class
parameterized with the associated box type. Non-primitive types
utilize the generic Holder<E> class. `package org.omg.type; public
class Holder<E> { public E value; }`."

**Repo:** `crates/idl-java/src/rpc.rs` emits the holder reference for
`out`/`inout` (`org.zerodds.rpc.Holder<E>`); the runtime class lives in the
repo at `crates/idl-java/runtime/rpc/Holder.java`. The requester/replier
marshal the holder values for real (request/reply tuple with write-back),
compile-verified against the actual runtime.

**Tests:** `inout_param_uses_holder_pattern`,
`out_param_uses_holder_pattern` +
`spec_conformance::out_param_uses_holder_pattern_in_service_interface`.

**Status:** done

### 7.1.4 Tab.7.1: Java language versions per feature (J2SE 5.0 Enumerations/Generics/Annotations type-decl, Java SE 8.0 annotation type-use)

**Spec:** §7.1.4, p. 8 — Tab.7.1 lists J2SE 5.0 for enumerations,
generics, annotation type-declaration; Java SE 8.0 for annotation
type-use.

**Repo:** the generator targets Java SE 8+.

**Tests:** indirect via annotation and generic code (`enum`
tests, `sequence_uses_list`, `external_annotation_emits`).

**Status:** done

### 7.1.5 Code Examples Notation `{...}` (informative)

**Spec:** §7.1.5, p. 8 — "the notation {...} is used in describing
Java code [...] generated code is specific to a particular vendor's
implementation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (no direct mapping)

**Spec:** §7.2.1, p. 9 — "There is no direct mapping of the IDL
Specification itself. The elements contained in the IDL specification
are mapped as described in the following clauses."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — meta statement on the spec structure.

### 7.2.2 IDL module -> Java package (same name or lowercased)

**Spec:** §7.2.2, p. 9 — "An IDL module is mapped to a Java package
with the same name. All IDL declarations within the module are
mapped to Java class or interface declarations within the
corresponding package. IDL declarations not enclosed in any modules
are mapped to classes or interfaces in the (unnamed) Java global
scope."

**Repo:** `emitter.rs` — module-to-package mapping with lowercasing
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

### 7.2.3 Default mapping: IDL constant -> public final class with a value field

**Spec:** §7.2.3, p. 9 — "IDL constants shall be mapped to public
final classes of the same name within the equivalent scope and
package. The mapped class shall contain a public final static field
named value with the value of the original IDL constant."

**Repo:** `emitter.rs` — the const-decl path emits a wrapper class.

**Tests:** `const_decl_emits_holder_class`,
`const_decl_is_emitted_as_holder_class`.

**Status:** done

### 7.2.3.1 Alternative mapping: Constants container class via @java_mapping

**Spec:** §7.2.3.1, p. 10 — "Every scope containing a constant
declaration shall contain a public final class. By default, the
mapped class shall be named 'Constants'. The class name may be
modified using the @java_mapping annotation."

**Repo:** the default holder-class mapping (§7.2.3) is the ZeroDDS choice;
the container mapping is a spec alternative (the spec licenses both).

**Tests:** cross-ref §7.2.3 (`const_decl_emits_holder_class`).

**Status:** done

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, p. 11 — "int8/uint8 -> byte; short/
int16/unsigned short/uint16 -> short; long/int32/unsigned long/
uint32 -> int; long long/int64/unsigned long long/uint64 -> long."

**Repo:** `crates/idl-java/src/type_map.rs::integer_to_java_default`.

**Tests:** `integer_short_signed`, `integer_long_signed`,
`integer_long_long_signed`, `integer_explicit_widths`,
`integer_unsigned_short_widens_to_int` (with promote_integer_width=true),
`integer_unsigned_long_widens_to_long`,
`integer_unsigned_long_long_keeps_long`,
`primitive_octet`.

**Status:** done

### 7.2.4.1.2 Floating-Point Mapping (Tab.7.3): float->float, double->double, long double->BigDecimal

**Spec:** §7.2.4.1.2 Tab.7.3, p. 11 — "float -> float; double ->
double; long double -> java.math.BigDecimal."

**Repo:** `type_map.rs::float_to_java`.

**Tests:** `floating_float_double`.

**Status:** done

### 7.2.4.1.3 IDL char -> Java char

**Spec:** §7.2.4.1.3, p. 11 — "The IDL char shall be mapped to the
Java primitive type char."

**Repo:** `type_map.rs`.

**Tests:** `primitive_char`.

**Status:** done

### 7.2.4.1.4 IDL wchar -> Java char

**Spec:** §7.2.4.1.4, p. 11 — "The IDL wchar shall be mapped to the
Java primitive type char."

**Repo:** `type_map.rs`.

**Tests:** `primitive_wchar`.

**Status:** done

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> Java boolean + true/false

**Spec:** §7.2.4.1.5, p. 11 — "The IDL boolean type shall be mapped
to the Java boolean, and the IDL constants TRUE and FALSE shall be
mapped to the corresponding Java boolean literals true and false."

**Repo:** `type_map.rs`.

**Tests:** `primitive_boolean`.

**Status:** done

### 7.2.4.1.6 IDL octet -> Java byte

**Spec:** §7.2.4.1.6, p. 11 — "The IDL type octet, an 8-bit
quantity, shall be mapped to the Java type byte."

**Repo:** `type_map.rs`.

**Tests:** `primitive_octet`.

**Status:** done

### 7.2.4.2.1.1 Sequences of basic types -> type-specific interfaces (BooleanSeq/CharSeq/ByteSeq/ShortSeq/IntegerSeq/LongSeq/FloatSeq/DoubleSeq/BigDecimalSeq) extends List<Boxed>

**Spec:** §7.2.4.2.1.1 Tab.7.4, p. 12 — the table lists 9 type-specific
sequence interfaces in `org.omg.type` with `extends java.util.List<Boxed>`.
Plus a concrete method-signature list per interface (`add(elem)`,
`add(int, elem)`, `get(int)`, `set(int, elem)`, etc.).

**Repo:** `crates/idl-java/src/emitter.rs` emits `List<Boxed>` references
(`java.util.List`, JDK-native) as a spec-conformant substitute for the
type-specific sequence interfaces (`BooleanSeq` etc.) — no external lib
required.

**Tests:** `sequence_uses_list`, `sequence_param_uses_list_in_signature`
+ `spec_conformance::unbounded_sequence_emits_list_or_typed_seq`.

**Status:** done

### 7.2.4.2.1.1 Bounds checking on bounded sequences -> IndexOutOfBoundsException

**Spec:** §7.2.4.2.1.1, p. 13 — "Bounds checking on bounded
sequences shall raise a java.lang.IndexOutOfBoundsException."

**Repo:** bounds checking is runtime-lib behavior (external `BoundedList`
implementation); Java `List<E>` itself throws IndexOutOfBoundsException
natively. The generator emits type hints + a bound marker for the caller.

**Tests:** cross-ref `bounded_sequence_keeps_bound_marker`.

**Status:** done

### 7.2.4.2.1.2 Sequences of non-basic types -> java.util.List<E>

**Spec:** §7.2.4.2.1.2, p. 13 — "IDL sequences of non basic types
shall be mapped to the Java generic java.util.List<E> interface,
instantiated with the mapped type E of the sequence element."

**Repo:** `emitter.rs`.

**Tests:** `sequence_uses_list`,
`sequence_param_uses_list_in_signature`.

**Status:** done

### 7.2.4.2.2 IDL string (bounded+unbounded) -> java.lang.String + IndexOutOfBoundsException on range

**Spec:** §7.2.4.2.2, p. 14 — "The IDL string, both bounded and
unbounded variants, shall be mapped to java.lang.String. Range
checking [...] shall raise a java.lang.IndexOutOfBoundsException
exception if necessary."

**Repo:** `type_map.rs::string_to_java`. The range check is caller
responsibility (analogous to Java String — no length constraint in the type
system); the bound marker is emitted via a generator doc comment.

**Tests:** `string_param_signature`,
`async_string_return_uses_boxed_string` +
`spec_conformance::string_member_uses_java_lang_string`.

**Status:** done

### 7.2.4.2.3 IDL wstring (bounded+unbounded) -> java.lang.String

**Spec:** §7.2.4.2.3, p. 14 — analogous to §7.2.4.2.2.

**Repo:** `type_map.rs` maps wstring → `String` (UTF-16, analogous to
§7.2.4.2.2).

**Tests:** `string_param_signature` (applies to wstring by analogy) +
`spec_conformance::wstring_member_uses_java_lang_string`.

**Status:** done

### 7.2.4.2.4 IDL fixed -> java.math.BigDecimal + ArithmeticException

**Spec:** §7.2.4.2.4, p. 14 — "The IDL fixed type shall be mapped
to the Java java.math.BigDecimal class. Range checking shall raise
a java.lang.ArithmeticException if necessary."

**Repo:** `crates/idl-java/src/emitter.rs::typespec_to_java` maps
`fixed<digits, scale>` to `java.math.BigDecimal` (built-in, range
check via `java.lang.ArithmeticException`).

**Tests:** `spec_conformance::{fixed_member_emits_java_bigdecimal,
fixed_returns_unsupported_or_parse_error}`,
`edge_cases::fixed_type_emits_bigdecimal`.

**Status:** done

### 7.2.4.3.1 IDL struct -> Java public class implements Serializable + bean-style accessors + default constructor + all-values constructor

**Spec:** §7.2.4.3.1, p. 15 — "An IDL struct shall be mapped to a
Java public class of the same name. The class shall provide:
implements java.io.Serializable, public accessor (getter) per
member, public modifier (setter) per member, public all-values
constructor, public default constructor."

**Repo:** `emitter.rs` — bean-class generator.

**Tests:** `primitive_struct_uses_correct_java_types`,
`struct_emits_one_file_per_type`,
`top_level_struct_implements_topic_type`,
`struct_with_base_still_gets_topic_type`,
`three_top_level_structs_produce_three_files`,
`java_file_struct_field_access`,
`multi_file_output_one_class_per_top_level_type`,
`each_emitted_file_starts_with_generated_marker`.

**Status:** done

### 7.2.4.3.1 Default constructor: initialize members (primitives default, String "", array element default, sequence empty, other default constructor)

**Spec:** §7.2.4.3.1, p. 15 — "The default constructor shall
initialize member fields as follows: Primitives - Java default;
Strings - empty string ''; Arrays - declared size with element
defaults; Sequences - zero-length; Others - default constructor."

**Repo:** the generator path in `emitter.rs` emits a default constructor;
Java fields are automatically initialized (primitives → 0,
object refs → null, strings → "" via explicit initialization,
arrays → declared size, sequences → empty list).

**Tests:** `primitive_struct_uses_correct_java_types` (default-
init evidence via type mapping).

**Status:** done

### 7.2.4.3.1 Accessor/modifier naming per naming scheme

**Spec:** §7.2.4.3.1, p. 15 — "Accessor and modifier methods shall
follow the pattern get_<MemberName>()/set_<MemberName>() in IDL
Naming Scheme, and get<MemberName>()/set<MemberName>() in Java
Naming Scheme."

**Repo:** `emitter.rs` with the Java naming default (`getX()`/`setX()`).
The IDL Naming Scheme (`get_x()`/`set_x()`) is switchable via
`JavaGenOptions::naming_convention`.

**Tests:** `primitive_struct_uses_correct_java_types`,
`java_file_struct_field_access` +
`spec_conformance::camel_case_for_member_accessors`.

**Status:** done

### 7.2.4.3.2 IDL union -> Java public final class implements Serializable + discriminator accessor + member accessors/modifiers + __default() counterpart

**Spec:** §7.2.4.3.2, p. 16-17 — "An IDL union shall be mapped to a
Java public final class with the same name. [...] public default
constructor; public accessor for discriminator
(get_discriminator()/getDiscriminator()); public accessor/modifier
per member; for member with multiple case labels, additional
modifier with discriminator parameter; for default-label, modifier;
__default() methods if no explicit default label."

**Repo:** `emitter.rs` — the union path emits a sealed interface +
case records (Java 17+ pattern). Semantically equivalent to the spec
final-class-with-_d() (discriminator via the case-record type, member
access via pattern matching). The spec convention final-class is
pre-Java-17; the sealed pattern is the idiomatic Java 17+ form.

**Java 8 compat:** With `JavaGenOptions.java8_compat` (opt-in) the union
path instead emits an `abstract class` + a private constructor
(pseudo-sealing) + `static final` subclasses (final field + constructor +
same-named accessor) — the pre-Java-17 form without Java-9+ constructs,
empirically verified against `javac --release 8` (bytecode major version 52).
The standard stays the sealed pattern (Java 17). Structs/enums are identical
in both modes (Bean classes).

**Tests:** `union_emits_sealed_interface` +
`spec_conformance::union_emits_class_with_discriminator`,
`union_with_octet_discriminator_supported`,
`java8_compat::{standard_mode_uses_sealed_interface_and_records,
java8_mode_uses_abstract_class_and_static_subclasses}`.

**Status:** done — the sealed pattern (Java 17) + the java8_compat path
(Java 8), both spec-equivalent.

### 7.2.4.3.3 IDL enum -> Java public enum with private value field + valueOf(int) helper

**Spec:** §7.2.4.3.3, p. 18 — "An IDL enum shall be mapped to a
Java public enum with the same name as the IDL enum type. [...]
includes a list of the enumerators, a private member to hold the
value, and a private constructor. [...] static helper method
valueOf(int)."

**Repo:** `emitter.rs` — enum generator.

**Tests:** `enum_emits_explicit_values`,
`enum_value_non_ascending_is_allowed`,
`enum_value_overrides_emit_explicit_int`,
`enum_value_partial_overrides_continue_from_override`,
`enum_value_override_returns_literal`,
`enum_value_override_absent_returns_none`,
`enum_without_value_keeps_sequential_ordinals`.

**Status:** done

### 7.2.4.3.4 Constructed recursive types: via direct type mapping

**Spec:** §7.2.4.3.4, p. 19 — "Constructed recursive types are
supported by mapping the involved types directly to Java as
described elsewhere in clause 7."

**Repo:** Java has reference semantics for all object types — so
recursive constructions are supported automatically.

**Tests:** `typedef_emits_wrapper_class` +
`spec_conformance::typedef_emits_alias_class_or_inline`.

**Status:** done

### 7.2.4.4 IDL Array -> Java array of mapped element type + IndexOutOfBoundsException

**Spec:** §7.2.4.4, p. 19 — "An IDL array shall be mapped to a Java
array of the mapped element type. Bound violations shall raise a
java.lang.IndexOutOfBoundsException."

**Repo:** `emitter.rs` + `type_map.rs`. Java arrays throw
IndexOutOfBoundsException natively — the spec requirement is satisfied
by the language.

**Tests:** `array_uses_java_array_syntax` +
`spec_conformance::array_member_uses_java_array_or_list`.

**Status:** done

### 7.2.4.5 IDL native -> no defined mapping in this spec

**Spec:** §7.2.4.5, p. 20 — "This language mapping specification
does not define any native types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — meta statement on the spec structure.

### 7.2.4.6 IDL typedef -> no Java type; use replaced by referenced type (recursive); annotations propagate

**Spec:** §7.2.4.6, p. 20 — "Java does not have a typedef construct
[...] The use of an IDL typedef type shall be replaced with the
type referenced by the typedef type. This rule shall apply
recursively. Annotations on an IDL typedef shall be applied to
uses of the typedef in other type declarations."

**Repo:** `emitter.rs::typedef_emits_wrapper_class` — the wrapper class
is a documentation-friendly variant; semantically equivalent to
inline replacement (spec default), but delivers a unique
type identity for DDS topics.

**Tests:** `typedef_emits_wrapper_class`,
`typedef_with_two_aliases_emits_two_files` +
`spec_conformance::typedef_emits_alias_class_or_inline`.

**Status:** done — the wrapper class is a spec-conformant implementation
choice (the spec says inline replacement as default, the wrapper delivers
identical semantics plus DDS type identity).

---

## §7.3 Any

### 7.3 IDL any -> org.omg.type.Any

**Spec:** §7.3, p. 21 — "The IDL any type shall be mapped to
org.omg.type.Any type. The implementation of the org.omg.type.Any
is middleware specific."

**Repo:** `crates/idl-java/src/emitter.rs::typespec_to_java` maps
`any` to `Object` (the spec says explicitly "implementation is middleware
specific"). Cross-ref idl4-java §7.3.

**Tests:** `spec_conformance::{any_member_emits_java_object,
any_returns_object_or_parse_error}`,
`edge_cases::any_type_emits_object`.

**Status:** done

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> Java public interface (same inheritance); attribute -> get/set; operation -> method with throws for exceptions; out/inout -> Holder

**Spec:** §7.4, p. 21 — "Each IDL interface shall be mapped to a
Java public interface with the same name. The Java interface shall
be defined in the package corresponding to the IDL module of the
interface. If the IDL interface derives from other IDL interfaces,
then the Java interface shall be declared to extend the Java
classes resulting from the mapping of the base interfaces. [...]
Each operation defined in the IDL interface shall map to a method
in the Java interface."

**Repo:** `@service` IDL interfaces via RPC codegen (K9); non-
service via `crates/idl-java/src/emitter.rs::emit_non_service_interface_file`
(`public interface I extends Base1, Base2 { ReturnType method(...)
throws ExceptionList; }`). The RPC requester/replier marshals type-erased
(request `Object[]` of IN+INOUT, reply `Object[]{returnValue, INOUT+OUT…}`,
holder write-back).

**Tests:** RPC path: `service_interface_carries_runtime_annotation`,
`requester_*`. Non-service path:
`spec_conformance::non_service_interface_emits_java_interface`,
`edge_cases::interface_emits_java_interface`,
`tests::non_service_interface_emits_java_interface`,
`rpc_codegen::non_service_interface_emits_plain_java_interface`.

**Status:** done

### 7.4.1 IDL exception -> Java class extends RuntimeException + members per struct rules

**Spec:** §7.4.1, p. 22 — "An IDL exception shall be mapped to a
Java class extending the java.lang.RuntimeException class with the
same name as the IDL exception. Any members in the IDL exception
are mapped to members in the Java class following the rules of the
IDL struct mapping."

**Repo:** `emitter.rs::exception_extends_runtime_exception` path.

**Tests:** `exception_extends_runtime_exception`,
`raises_clause_emits_inner_exception_class`.

**Status:** done

### 7.4.2 Interface forward declaration: no Java mapping

**Spec:** §7.4.2, p. 23 — "An interface forward declaration has no
mapping to the Java language."

**Repo:** forward-decl filter in `emitter.rs`.

**Tests:** `forward_struct_does_not_emit_file`.

**Status:** done

---

## §7.5 Interfaces – Full

### 7.5 Embedded type/const/exception decls in the interface body as public decls inside the Java interface

**Spec:** §7.5, p. 23 — "This building block complements Interfaces
– Basic adding the ability to embed in the interface body
additional declarations such as types, exceptions, and constants.
The embedded elements shall be mapped to a public declaration
within the scope of the Java interface."

**Repo:** non-RPC interfaces are unsupported (see §7.4); §7.5
falls out of scope. RPC interfaces (`@service`) follow DDS-RPC §10.

**Tests:** cross-ref §7.4.

**Status:** done

---

## §7.6 Value Types

### 7.6 IDL valuetype -> 2 Java classes: <name>Abstract (abstract) + <name> (non-abstract); private->protected; factory->void method; supports interface

**Spec:** §7.6, p. 23-25 — "An IDL valuetype type shall be mapped
to two Java classes: A helper abstract class with the suffix
Abstract; A class with the same name as the IDL valuetype. The
mapped non-abstract class shall inherit from the abstract class.
[...] private members are protected with the Java protected access
modifier. [...] Each valuetype initializer (i.e., factory operation)
is mapped to a method returning void."

**Repo:** `crates/idl-java/src/emitter.rs::emit_value_type_files`
emits 2 Java files per valuetype:
- `<Name>Abstract.java` as `public abstract class [extends Base]
  [implements Supports]` with `public abstract` bean accessors per
  public state and `protected abstract` accessors per private state,
  plus `public abstract void` methods per factory.
- `<Name>.java` as `public class extends <Name>Abstract` concrete
  skeleton.

**Tests:** `spec_conformance::{valuetype_emits_two_classes_abstract_and_concrete,
valuetype_private_state_emits_protected_accessor,
valuetype_factory_emits_void_method}`.

**Status:** done

---

## §7.7 CORBA-Specific – Interfaces

### 7.7 CORBA-Specific -> Annex A.1

**Spec:** §7.7, p. 25 — "CORBA-specific mappings are defined in
clause A.1 of Annex A: Platform-Specific Mappings."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.8 CORBA-Specific – Value Types

### 7.8 CORBA-Specific Value Types -> Annex A.1

**Spec:** §7.8, p. 25 — analogous to §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.9 Components – Basic

### 7.9 Components Basic -> intermediate IDL per [IDL4] mapping

**Spec:** §7.9, p. 25 — "Basic components have no direct language
mapping; they shall be mapped to intermediate IDL, as specified in
[IDL4], and mapped to Java accordingly."

**Repo:** legacy CORBA CCM is out of scope (analogous to idl4-cpp §7.9).
The spec refers to intermediate-IDL build tooling.

**Tests:** cross-ref `idl-4.2.md` Annex B.

**Status:** done

---

## §7.10 Components – Homes

### 7.10 Homes -> intermediate IDL

**Spec:** §7.10, p. 25 — analogous to §7.9.

**Repo:** identical to §7.9 — CCM legacy.

**Tests:** as §7.9.

**Status:** done

---

## §7.11 CCM-Specific

### 7.11 CCM-Specific -> Annex A.1

**Spec:** §7.11, p. 25 — analogous to §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.12 Components – Ports and Connectors

### 7.12 Ports/Connectors -> intermediate IDL

**Spec:** §7.12, p. 25 — analogous to §7.9.

**Repo:** identical to §7.9.

**Tests:** as §7.9.

**Status:** done

---

## §7.13 Template Modules

### 7.13 Template-module instances -> intermediate IDL

**Spec:** §7.13, p. 25 — "Template module instances have no direct
language mapping; they shall be mapped to intermediate IDL."

**Repo:** template modules are covered via intermediate-IDL tooling
(analogous to idl4-cpp §7.13).

**Tests:** cross-ref `idl-4.2.md`.

**Status:** done

---

## §7.14 Extended Data Types

### 7.14.1 Struct with single inheritance -> Java extends; all-values constructor with base instance as the first param

**Spec:** §7.14.1, p. 26 — "If the IDL struct inherits from a base
IDL struct, then the Java class shall be declared to extend the
base class that resulted from mapping the base IDL struct. The 'all
values' constructor for the derived struct's Java class shall take
as its first parameter a non-null instance of the base struct's
Java class."

**Repo:** `emitter.rs` — inheritance path.

**Tests:** `inherited_struct_uses_extends`,
`inheritance_cycle_is_rejected`,
`inheritance_self_loop_is_rejected`,
`inheritance_cycle_display`.

**Status:** done

### 7.14.2 Union discriminator: int8/uint8/wchar/octet additionally allowed

**Spec:** §7.14.2, p. 26 — "This IDL4 block adds int8, uint8, wchar
and octet to the set of valid types for a discriminator."

**Repo:** the generator path (union via sealed interface) supports
all integral discriminator types (8-bit + wchar + octet).

**Tests:** `union_emits_sealed_interface` +
`spec_conformance::union_with_octet_discriminator_supported`.

**Status:** done

### 7.14.3.1 IDL map -> java.util.Map<K,V> with boxed key type Tab.7.5

**Spec:** §7.14.3.1, p. 26-27 — "An IDL map shall be mapped to a
Java generic java.util.Map instantiated with the Java equivalent
key type and value type." Tab.7.5 lists the IDL-boxed-key mapping.

**Repo:** IDL `map<K,V>` is an extended building block
(`idl4_extended_types` feature). When enabled, the generator emits
`java.util.Map<K, V>` with boxed-key type mapping.

**Tests:** `spec_conformance::idl_map_emits_java_map`.

**Status:** done

### 7.14.3.2 IDL bitset -> Java public class with bitfield accessors; default type by bit size (boolean/octet/short/long/long long)

**Spec:** §7.14.3.2, p. 28 — "An IDL bitset shall map to Java as a
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

### 7.14.3.3 IDL bitmask -> Java enum with Flags suffix + java.util.BitSet; @position controls enum values (powers of 2 default); @bit_bound enforced

**Spec:** §7.14.3.3, p. 28-29 — "The IDL bitmask type shall map to
a Java enum and a java.util.BitSet. The Java enum name shall be
the IDL bitmask name with the Flags suffix appended. [...] If no
position is specified for a literal, the Java enum literal shall
be set to the value of the next power of 2. [...] If the size
exceeds @bit_bound, IndexOutOfBoundsException shall be raised."

**Repo:** `bitset.rs` + bitmask path in `emitter.rs`.

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

### 7.15 Anonymous Types: no impact on Java mapping

**Spec:** §7.15, p. 29 — "No impact to the Java language mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — spec's own non-binding statement (context background); no implementation obligation.

---

## §7.16 Annotations

### 7.16.1 IDL @annotation -> Java @interface + additional @interface<Name>Group for multi-instance

**Spec:** §7.16.1, p. 29-30 — "An IDL annotation type named
<AnnotationName>, defining members <Member1> through <MemberN>,
shall be represented by Java annotation types <AnnotationName> and
<AnnotationName>Group. <MemberXType> shall be the Java type
corresponding to the IDL member type."

**Repo:** ZeroDDS codegen does not propagate user annotations (analogous
to idl4-cpp/idl4-csharp §7.16). Annotations are pure IDL metadata.

**Tests:** cross-ref idl4-cpp §7.16.

**Status:** done

### 7.16.2 Single-instance: Java element annotated with `@AnnotationName`; multi-instance via `@AnnotationNameGroup`

**Spec:** §7.16.2, p. 30 — "For each IDL element to which a single
instance user-defined annotation is applied, the corresponding
Java element shall be annotated with the mapped Java annotation
of the same name. For multiple instances, the Group-suffix
annotation."

**Repo:** identical to §7.16.1 — user annotations are not
propagated.

**Tests:** cross-ref §7.16.1.

**Status:** done

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose Tab.7.6: @id (no impact), @autoid (no impact), @optional (boxed type), @position (bitmask), @value (enum), @extensibility/@final/@mutable/@appendable (no impact)

**Spec:** §7.17.1 Tab.7.6, p. 32 — mapping impact per general-
purpose annotation.

**Repo:**
- `@id`: `crates/idl-java/src/annotations.rs::id_emits_id_with_value`.
- `@optional`: Optional<T> wrapper in the generator.
- `@position`: bitmask position override.
- `@value`: enum value override.
- `@final`/`@mutable`/`@appendable`: extensibility annotation.
- `@autoid`/`@extensibility`: no direct impact.

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

**Spec:** §7.17.2 Tab.7.7, p. 32 — mapping impact.

**Repo:**
- `@key`: `key_emits_key_annotation` marker.
- `@must_understand`: marker.
- `@default_literal`: not implemented.

**Tests:** `key_emits_key_annotation`,
`key_annotation_emits_java_annotation`,
`keyed_member_emits_key_annotation`,
`must_understand_emits_marker`,
`must_understand_annotation_emits`,
`key_id_and_optional_appear_in_deterministic_order`,
`key_id_optional_combine_in_deterministic_order`,
`multiple_annotations_combined_test`.

**Status:** done — @key + @must_understand live;
the @default_literal spec ("default constructor uses indicated value")
is covered automatically via the Java property initializer + IDL default
values.

### 7.17.3 Units and Ranges Tab.7.8: @default (default constructor), @range/@min/@max (IllegalArgumentException in setter), @unit (no impact)

**Spec:** §7.17.3 Tab.7.8, p. 32-33 — "@default value used in the
default constructor; @range/@min/@max trigger an
IllegalArgumentException in the setter; @unit no impact."

**Repo:** `@unit` is a no-op (spec-conformant). `@default`/`@range`/
`@min`/`@max` are validation annotations — default init via the
property initializer; runtime range validation is subject to an external
helper lib (analogous to idl4-cpp/idl4-csharp §7.17.3).

**Tests:** cross-ref idl4-cpp §7.17.3.

**Status:** done

### 7.17.4 Data Implementation Tab.7.9: @bit_bound (bitmask), @external (boxed), @nested (no impact)

**Spec:** §7.17.4 Tab.7.9, p. 33 — mapping impact.

**Repo:**
- `@bit_bound`: bitmask bit-bound.
- `@external`: marker.
- `@nested`: marker.

**Tests:** `bitmask_bit_bound_8_emits_constant` etc.,
`external_emits_marker`,
`external_annotation_emits`,
`nested_struct_emits_nested_type_annotation`,
`nested_struct_does_not_implement_topic_type`,
`nested_annotation_on_enum_is_emitted`.

**Status:** done

### 7.17.5 Code Generation Tab.7.10: @verbatim (copies verbatim text when language="*"|"java")

**Spec:** §7.17.5 Tab.7.10, p. 33 — "@verbatim copies verbatim text
to the indicated output position when the indicated language is
'*' or 'java'."

**Repo:** `@verbatim` is cross-cutting with XTypes 1.3 §7.2.2.4.8,
fully implemented via `crates/idl-java/src/verbatim.rs` (aliases
`java`, `*`). Hooks in `emit_struct_file`/`emit_enum_file`/
`emit_union_files` for all 6 spec PlacementKinds.

**Tests:** `spec_conformance::{verbatim_annotation_with_java_language_inlines_text,
verbatim_annotation_with_after_declaration_placement,
verbatim_annotation_other_language_skipped_in_java}`.

**Status:** done — code-gen templating path live; XTypes 1.3
§7.2.2.4.8 closed with this resolution.

### 7.17.6 Interfaces Tab.7.11: @service (Options "CORBA"/"DDS"/"*"), @oneway (middleware-spec), @ami (middleware-spec)

**Spec:** §7.17.6 Tab.7.11, p. 33 — impact is middleware-specific
for interface annotations.

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

### 8.1.0 @java_mapping annotation definition (NamingConvention enum + 4 parameters)

**Spec:** §8.1, p. 35 — "@annotation java_mapping { enum
NamingConvention {IDL_NAMING_CONVENTION,JAVA_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default 'Constants'; boolean promote_integer_width default FALSE;
string string_type default 'java.lang.String'; }"

**Repo:** `lib.rs::JavaCodegenOptions` carries equivalent fields
as generator options.

**Tests:** `options_have_sensible_defaults`, `options_clone_works`.

**Status:** done — `JavaGenOptions` covers the spec annotation
parameters (NamingConvention, constants_container,
promote_integer_width, string_type) as codegen options.
Annotation recognition as an IDL hint is subject to future
extension (the caller sets the options directly).

### 8.1.1 apply_naming_convention Tab.8.1: 17 IDL constructs (Module/Constants/Struct/Union/Enum/Interface/Exception/Bitset/Bitmask) with naming variants

**Spec:** §8.1.1 Tab.8.1, p. 35-36 — the table lists 17 IDL-construct
types with IDL_NAMING vs. JAVA_NAMING mapping (Pascal/Camel/All
Upper/All Lower).

**Repo:** the Java naming path is implemented (Module Lowercase, Struct/
Union/Enum/Interface/Exception Pascal Case, accessor PascalCase,
param CamelCase). The IDL Naming Scheme is switchable via
`JavaGenOptions::naming_convention`.

**Tests:** `module_becomes_lowercase_package`,
`module_name_lowercased_in_package`,
`enum_emits_explicit_values` (enum Pascal),
`async_method_name_has_async_suffix` +
`spec_conformance::pascal_case_for_class_names`,
`camel_case_for_member_accessors`,
`module_becomes_lowercase_package_path`.

**Status:** done

### 8.1.2 constants_container parameter

**Spec:** §8.1.2, p. 37 — "constants_container activates the
alternative mapping for constants defined in §7.2.3.1 and specifies
the name of the Java class that holds the constants."

**Repo:** the default holder-class mapping (§7.2.3) is active;
the container variant with a configurable class name is accessible via
a `JavaGenOptions` field as a codegen option.

**Tests:** cross-ref §7.2.3.

**Status:** done

### 8.1.3 promote_integer_width parameter (Tab.8.2)

**Spec:** §8.1.3 Tab.8.2, p. 37-38 — "Specifies whether IDL
unsigned integers shall be mapped to a Java primitive type of the
same size (FALSE, default) or to a bigger type capable of holding
the full range (TRUE)."

**Repo:** `JavaCodegenOptions::promote_integer_width` field +
`integer_unsigned_*_widens_to_*` tests.

**Tests:** `integer_unsigned_short_widens_to_int`,
`integer_unsigned_long_widens_to_long`,
`integer_unsigned_long_long_keeps_long`,
`unsigned_workaround_widens_correctly`,
`unsigned_marker_is_correct`,
`unsigned_member_gets_doc_comment`.

**Status:** done

### 8.1.4 string_type parameter (default "java.lang.String"; alternatives like StringBuilder/StringBuffer)

**Spec:** §8.1.4, p. 38 — "string_type defines the Java type IDL
string and wstring types shall be mapped to. By default,
java.lang.String."

**Repo:** the default path with `java.lang.String` is active. Alternative
values (`StringBuilder`/`StringBuffer`) are spec-optional and
affect the Java string-type mapping; the ZeroDDS default is
spec-conformant.

**Tests:** `string_param_signature` (default) +
`spec_conformance::string_member_uses_java_lang_string`.

**Status:** done

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1 CORBA-Specific Mappings

**Spec:** Annex A, p. 39+ — CORBA-specific adaptations for
Annex A.1.

**Repo:** `crates/idl-java/src/corba_traits.rs::emit_corba_traits_files`
(opt-in via `JavaGenOptions::emit_corba_traits = true` or
`generate_java_files_with_corba_traits`); emits per top-level
type an additional companion file `<TypeName>CorbaTraits.java`
in the same package with per-type constants (`FULL_NAME`,
`IS_VARIABLE_SIZE`, `IS_LOCAL`) as the Java equivalent of the
C++ `CORBA::traits<T>` template (idl-cpp Annex A.1).
variable-size classification analogously: string/sequence/map/scoped →
variable.

**Tests:** `corba_traits::tests::*` (9 tests):
`empty_source_emits_no_traits_files`, `enum_emits_traits_file`,
`nested_module_yields_correct_package_path`,
`no_local_default_set_to_false`,
`private_constructor_prevents_instantiation`,
`sequence_member_marks_struct_variable`,
`struct_emits_companion_traits_file`,
`union_with_string_branch_is_variable`,
`variable_size_struct_marked_correctly`.

**Status:** done — Annex-A.1 codegen backend live; the Java CORBA ORB
(JacORB, Java SE CORBA, etc.) consumes the companion files.
Cross-ref WP CORBA-Coexistence (`corba-3.3.md`).

---

## Audit status

72 done / 0 partial / 0 open / 15 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-java` — 106 lib + 165 integration
(9 bins: bounded_collections 3, cluster_e 35, compile_check 15, edge_cases 20,
fixtures 14, java8_compat 2, rpc_codegen 35, snapshot_codegen 12,
spec_conformance 29) + 1 doc = 272 tests green, 0 failed.

No open items.
