# IDL4 to Java Language Mapping 1.0 — Spec Coverage

**Spec:** [OMG IDL4-Java 1.0](https://www.omg.org/spec/IDL4-Java/1.0/PDF) (OMG formal/2021-08-01)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** the IDL4-Java generator `crates/idl-java` was removed in
`d045e43ea86`. The live Java column sits today in
`crates/lang-rules/src/adapter/java.rs`,
`crates/lang-rules/src/emit/java.rs`,
`crates/lang-rules/src/emit/interface_java.rs`,
`crates/lang-rules/src/default_java.rs`, plus
`crates/lang-cst/src/java/mod.rs` and `crates/lang-render/src/java.rs`.
It is a Plain Language Binding for XCDR2, described in
`docs/spec-coverage/zerodds-xcdr2-java-1.0.md`. In several places it
deliberately and with stated reasons departs from this mapping spec
(`fixed` as `String` instead of `BigDecimal`, `octet` as `int` instead of
`byte`, `map` as an entry array instead of `java.util.Map`, enumeration as
`int` constants instead of a Java `enum`, module as a nested class instead
of a package). Each of those departures is a finding here, not a `done`.
From the old crate only the runtime `crates/idl-java/runtime/` survives
(`org.zerodds.types.*` annotations and `org.zerodds.rpc.*`); today's
generator does not use it.

---

## §1 Scope

### 1.1 Mapping IDL v4 -> Java

**Spec:** §1, p. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 [IDL4] to the Java programming
language. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification with the
exception of middleware specific constructs that are better
addressed in separate specifications."

**Repo:** —

**Tests:** —

**Status:** `open` — no generator maps IDL to the Java forms of clause 7.
The Java column produces a wire binding whose own documentation names the
departures from this spec.

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

**Repo:** —

**Tests:** —

**Status:** `open` — the Java column transforms IDL into Java source, but by
the XCDR2 binding's PSM, not by clause 7.

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

**Repo:** —

**Tests:** —

**Status:** `open` — `@java_mapping` does not exist in the tree
(`git grep java_mapping` finds nothing), and there is no switchable
naming-scheme selection.

### 7.1.1.0 Name collision with reserved names: underscore prefix

**Spec:** §7.1.1, p. 5 — "if a mapped name or identifier collides
with one of the names reserved in Clause 7.1.2, the collision shall
be resolved by prepending an underscore ('_') to the mapped name."

**Repo:** `crates/lang-cst/src/java/mod.rs::escape_keyword`,
`crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-cst/src/java/mod.rs::tests::a_keyword_escapes_with_a_trailing_underscore`,
`crates/lang-rules/src/adapter/java.rs::tests::an_identifier_that_is_a_java_keyword_is_escaped`.

**Status:** `partial` — Java keywords are escaped, but with a **trailing**
`_` (`class_`) instead of the prescribed leading one (`_class`). The
reserved-name list of §7.1.2 is also incomplete (see there).

### 7.1.1.1 IDL Naming Scheme: names without case transformation

**Spec:** §7.1.1.1, p. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers without case transformation,
maintaining the original IDL names."

**Repo:** —

**Tests:** —

**Status:** `open` — the Java column does keep IDL names unchanged
(`a_module_is_a_nested_class_and_two_modules_do_not_collide`), but it is
not a naming scheme and identity with the IDL naming scheme is neither
intended nor tested.

### 7.1.1.2 Java Naming Scheme: JavaBeans 1.01 conventions

**Spec:** §7.1.1.2, p. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers that follow the coding
guidelines defined in the JavaBeans 1.01 [JavaBeans] specification."

**Repo:** —

**Tests:** —

**Status:** `open` — getX/setX accessors and the JavaBeans conventions are
not generated.

### 7.1.1.2.1 Pascal Case Transformation Rules

**Spec:** §7.1.1.2.1, p. 5-6 — "The first letter after each
underscore shall be capitalized and all underscores shall be removed.
The first letter of the IDL name shall be capitalized."

**Repo:** —

**Tests:** —

**Status:** `open` — no Pascal-case transformation for type and member names.

### 7.1.1.2.2 Camel Case Transformation Rules

**Spec:** §7.1.1.2.2, p. 6 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be lower case."

**Repo:** —

**Tests:** —

**Status:** `open` — the only camel-case conversion in the tree is
`Java::camel` in `crates/lang-rules/src/emit/java.rs` for wire method
names; member names are not transformed.

### 7.1.1.2.3 All Uppercase Transformation Rules

**Spec:** §7.1.1.2.3, p. 6 — "Every letter shall be capitalized and
all underscores shall be removed."

**Repo:** —

**Tests:** —

**Status:** `open` — constants keep their IDL spelling; no uppercase
transformation.

### 7.1.1.2.4 All Lowercase Transformation Rules

**Spec:** §7.1.1.2.4, p. 6-7 — "Every letter shall be in lower case
and all underscores shall be removed."

**Repo:** —

**Tests:** —

**Status:** `open` — module names are not lowercased; a module `a`
becomes the nested class `a`, a module `common` the class `common`, but
without the spec transformation.

### 7.1.1.3 Suffixes (e.g. Abstract); underscore prefix on collision

**Spec:** §7.1.1.3, p. 7 — "If an IDL name ends in a reserved suffix
(for example, Abstract), then an underscore is prepended to the
mapped name."

**Repo:** —

**Tests:** —

**Status:** `open` — no reserved-suffix handling; there is no
`Abstract`-suffix mapping (the valuetype mapping is absent anyway).

### 7.1.2 Reserved Names

**Spec:** §7.1.2, p. 7 — "The mapping in effect reserves the use of
several names: <type>Abstract, Constants (per <moduleName>-Package);
all Java keywords; literals (true/false/null); java.lang.Object
methods (clone/equals/getClass/hashCode/notify/notifyAll/finalize/
toString/wait)."

**Repo:** `crates/lang-cst/src/java/mod.rs` — the keyword list.

**Tests:** `crates/lang-cst/src/java/mod.rs::tests::a_keyword_escapes_with_a_trailing_underscore`.

**Status:** `partial` — Java keywords plus `true`/`false`/`null` are
listed. `<type>Abstract`, `Constants` and the `java.lang.Object` methods
are missing.

### 7.1.3 Holder<E> class for inout/out parameters

**Spec:** §7.1.3, p. 8 — "Holder types are required in cases when
an IDL defined data type is passed to an operation as an inout or
out parameter. [...] `package org.omg.type; public class Holder<E>
{ public E value; }`."

**Repo:** `crates/lang-rules/src/emit/interface_java.rs`.

**Tests:** —

**Status:** `open` — there is no class `org.omg.type.Holder<E>`; the former
runtime class `crates/idl-java/runtime/rpc/Holder.java` was removed with
`b7589e6b904`. The RPC binding carries out/inout in an `_Out` type named by
DDS-RPC (`crates/lang-rules/src/emit/interface_java.rs`).

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

**Repo:** `crates/lang-rules/src/adapter/java.rs`,
`crates/lang-cst/src/java/mod.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_module_is_a_nested_class_and_two_modules_do_not_collide`,
`crates/lang-rules/src/adapter/java.rs::tests::a_module_reopened_beside_a_nested_one_is_one_class`.

**Status:** `partial` — modules are mapped to **nested classes** in one
file, not to packages (the crate justifies this with the one-public-type-
per-file rule). Two same-named types in different modules therefore do not
collide, but the spec form (package name) is not reached.

### 7.2.3 Default mapping: IDL constant -> public final class with a value field

**Spec:** §7.2.3, p. 9 — "IDL constants shall be mapped to public
final classes of the same name within the equivalent scope and
package. The mapped class shall contain a public final static field
named value."

**Repo:** —

**Tests:** —

**Status:** `open` — an IDL constant becomes a `public static final`
**field** in the enclosing scope, not a same-named wrapper class with
`value`.

### 7.2.3.1 Alternative mapping: Constants container class

**Spec:** §7.2.3.1, p. 10 — "Every scope containing a constant
declaration shall contain a public final class. By default, the
mapped class shall be named 'Constants'. The class name may be
modified using the @java_mapping annotation."

**Repo:** —

**Tests:** —

**Status:** `open` — no `Constants` container and no `constants_container`
parameter.

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, p. 11 — "int8/uint8 -> byte; short/
int16/unsigned short/uint16 -> short; long/int32/unsigned long/
uint32 -> int; long long/int64/unsigned long long/uint64 -> long."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`,
`crates/lang-rules/tests/one_walk_two_targets.rs::an_unsigned_local_is_wide_enough_in_java`.

**Status:** `partial` — `int8 -> byte`, `short -> short`,
`long -> int` and `long long -> long` match. The unsigned types depart
deliberately: `uint8`/`octet -> int` (spec: `byte`), `unsigned short ->
int` (spec: `short`), `unsigned long -> long` (spec: `int`), because the
column wants to hold the full value range.

### 7.2.4.1.2 Floating-Point Mapping (Tab.7.3)

**Spec:** §7.2.4.1.2 Tab.7.3, p. 11 — "float -> float; double ->
double; long double -> java.math.BigDecimal."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — `float -> float` and `double -> double` match.
`long double` becomes `org.zerodds.wire.Float128`, not `BigDecimal`.

### 7.2.4.1.3 IDL char -> Java char

**Spec:** §7.2.4.1.3, p. 11 — "The IDL char shall be mapped to the
Java primitive type char."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** done

### 7.2.4.1.4 IDL wchar -> Java char

**Spec:** §7.2.4.1.4, p. 11 — "The IDL wchar shall be mapped to the
Java primitive type char."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** done

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> Java boolean + true/false

**Spec:** §7.2.4.1.5, p. 11 — "The IDL boolean type shall be mapped
to the Java boolean, and the IDL constants TRUE and FALSE shall be
mapped to the corresponding Java boolean literals true and false."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — `boolean` becomes `boolean` (test above); no test checks that the constants `TRUE`/`FALSE` become `true`/`false`.

### 7.2.4.1.6 IDL octet -> Java byte

**Spec:** §7.2.4.1.6, p. 11 — "The IDL type octet, an 8-bit
quantity, shall be mapped to the Java type byte."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — `octet` becomes `int`, not `byte`; the column
justifies this with the range 0..255, which a signed `byte` cannot hold.

### 7.2.4.2.1.1 Sequences of basic types -> type-specific interfaces

**Spec:** §7.2.4.2.1.1 Tab.7.4, p. 12 — type-specific sequence interfaces
in `org.omg.type` with `extends java.util.List<Boxed>`.

**Repo:** —

**Tests:** —

**Status:** `open` — no `BooleanSeq`/`CharSeq`/… interfaces; a sequence
becomes a Java array.

### 7.2.4.2.1.1 Bounds checking on bounded sequences

**Spec:** §7.2.4.2.1.1, p. 13 — "Bounds checking on bounded
sequences shall raise a java.lang.IndexOutOfBoundsException."

**Repo:** —

**Tests:** —

**Status:** `open` — there is no bounded-sequence type. A Java array
throws `IndexOutOfBoundsException` natively, but the spec form (bounded
sequence interfaces) is absent.

### 7.2.4.2.1.2 Sequences of non-basic types -> java.util.List<E>

**Spec:** §7.2.4.2.1.2, p. 13 — "IDL sequences of non basic types
shall be mapped to the Java generic java.util.List<E> interface."

**Repo:** —

**Tests:** —

**Status:** `open` — a sequence becomes a Java array `T[]`, not
`java.util.List<E>`.

### 7.2.4.2.2 IDL string -> java.lang.String + IndexOutOfBoundsException

**Spec:** §7.2.4.2.2, p. 14 — "The IDL string, both bounded and
unbounded variants, shall be mapped to java.lang.String. Range
checking [...] shall raise a java.lang.IndexOutOfBoundsException
exception if necessary."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`.

**Status:** `partial` — `string` becomes `java.lang.String` (evidenced in
the test as an element type). The range checking is missing; the bound is
not enforced.

### 7.2.4.2.3 IDL wstring -> java.lang.String

**Spec:** §7.2.4.2.3, p. 14 — analogous to §7.2.4.2.2.

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`.

**Status:** `partial` — `wstring` and `string` share `java.lang.String`
(UTF-16). The range checking is missing.

### 7.2.4.2.4 IDL fixed -> java.math.BigDecimal + ArithmeticException


**Spec:** §7.2.4.2.4, p. 14 — "The IDL fixed type shall be mapped
to the Java java.math.BigDecimal class. Range checking shall raise
a java.lang.ArithmeticException if necessary."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_fixed_is_a_bigdecimal`, `crates/lang-rules/tests/java_runtime_selftest.rs`

**Status:** `partial` — `fixed<P,S>` becomes `java.math.BigDecimal` (CORBA 3.3 part 3 §1.5.6); the codec writes and reads the decimal digits through `Writer.writeFixed` and `Reader.readFixed`. The range check throws `WireException.OutOfRange`, not `java.lang.ArithmeticException`; that is the remainder. Log: `.agent/REPORT.md`.

### 7.2.4.3.1 IDL struct -> Java public class + Serializable + bean accessors + constructors

**Spec:** §7.2.4.3.1, p. 15 — "An IDL struct shall be mapped to a
Java public class of the same name. The class shall provide:
implements java.io.Serializable, public accessor per member, public
modifier per member, public all-values constructor, public default
constructor."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — a nested class with `public` fields is produced.
Missing: `implements java.io.Serializable`, accessors/modifiers, the
all-values constructor and the public default constructor.

### 7.2.4.3.1 Default constructor: initialise members

**Spec:** §7.2.4.3.1, p. 15 — "The default constructor shall
initialize member fields as follows: Primitives - Java default;
Strings - empty string; Arrays - declared size with element defaults;
Sequences - zero-length; Others - default constructor."

**Repo:** `crates/lang-rules/src/default_java.rs`.

**Tests:** —

**Status:** `open` — there is no public default constructor with this
initialisation. `default_java.rs` provides a `zd_def_<T>()` defaulter for
the decoder (DDS-XTypes Table 9), not the spec constructor.

### 7.2.4.3.1 Accessor/modifier naming per naming scheme

**Spec:** §7.2.4.3.1, p. 15 — "Accessor and modifier methods shall
follow the pattern get_<MemberName>()/set_<MemberName>() in IDL
Naming Scheme, and get<MemberName>()/set<MemberName>() in Java
Naming Scheme."

**Repo:** —

**Tests:** —

**Status:** `open` — no accessors are generated.

### 7.2.4.3.2 IDL union -> Java public final class + discriminator accessor

**Spec:** §7.2.4.3.2, p. 16-17 — "An IDL union shall be mapped to a
Java public final class with the same name. [...] public accessor for
discriminator [...] public accessor/modifier per member [...]"

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`.

**Status:** `partial` — the union becomes a class with an `_d` field and
one `public` field per case. Accessors/modifiers, the `getDiscriminator()`
access and the `__default()` forms are missing.

### 7.2.4.3.3 IDL enum -> Java public enum + valueOf(int)


**Spec:** §7.2.4.3.3, p. 18 — "An IDL enum shall be mapped to a
Java public enum with the same name [...] private member to hold the
value [...] static helper method valueOf(int)."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_enumeration_is_a_class_of_instances_and_a_member_of_it_is_one`, `crates/lang-rules/src/adapter/java.rs::tests::an_explicit_enumerator_value_is_written_out`, `crates/lang-rules/tests/java_map_class.rs::an_enumeration_is_a_class_of_instances_with_value_and_from_int`

**Status:** `partial` — the enumeration becomes a class with one instance per enumerator, the constants `public static final int _NAME`, `value()` and `from_int(int)` (CORBA 3.3 part 3 §1.5.9); a member is an instance of the class. A Java `enum` and the `valueOf(int)` of the spec are missing: the class carries the value an `enum` would not have for values the reader's IDL does not know. Log: `.agent/REPORT.md`.

### 7.2.4.3.4 Constructed recursive types

**Spec:** §7.2.4.3.4, p. 19 — "Constructed recursive types are
supported by mapping the involved types directly to Java."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/tests/java_corpus_compiles.rs::every_corpus_case_that_renders_also_compiles`.

**Status:** `partial` — Java reference semantics carry recursive types, and
the corpus contains them; a test that names the recursion is missing (the
one cited runs only with `--include-ignored`).

### 7.2.4.4 IDL array -> Java array + IndexOutOfBoundsException

**Spec:** §7.2.4.4, p. 19 — "An IDL array shall be mapped to a Java
array of the mapped element type. Bound violations shall raise a
java.lang.IndexOutOfBoundsException."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`.

**Status:** done

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

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_typedef_declares_nothing_and_says_so`.

**Status:** `partial` — the `typedef` declares nothing and its use takes
the target type (test). The propagation of annotations to the uses is
missing.

---

## §7.3 Any

### 7.3 IDL any -> org.omg.type.Any

**Spec:** §7.3, p. 21 — "The IDL any type shall be mapped to
org.omg.type.Any type. The implementation of the org.omg.type.Any
is middleware specific."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_construct_with_no_java_data_form_is_named_and_not_left_out`.

**Status:** `open` — `any` has no Java form in this column and is refused
by name, not mapped to an `Any` type.

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> Java public interface

**Spec:** §7.4, p. 21 — "Each IDL interface shall be mapped to a
Java public interface with the same name. [...] Each operation
defined in the IDL interface shall map to a method in the Java
interface."

**Repo:** `crates/lang-rules/src/emit/interface_java.rs`,
`crates/lang-rules/src/adapter/interface.rs`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::java_declares_the_same_entities_as_cpp`,
`crates/lang-rules/tests/interface_bindings.rs::java_returns_a_future_of_the_boxed_result`,
`crates/lang-rules/tests/interface_bindings.rs::java_returns_the_out_type_where_cpp_takes_a_reference`,
`crates/lang-rules/tests/interface_bindings.rs::java_declares_no_throws_and_still_names_the_raises`.

**Status:** `partial` — `@service` interfaces become `public interface`
with methods and attribute accessors; but the binding follows DDS-RPC 1.0
§7.11.2 (no `throws`, out/inout in `_Out` types). The base path for
non-service interfaces is missing.

### 7.4.1 IDL exception -> Java class extends RuntimeException

**Spec:** §7.4.1, p. 22 — "An IDL exception shall be mapped to a
Java class extending the java.lang.RuntimeException class with the
same name as the IDL exception. Any members [...] following the
rules of the IDL struct mapping."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** —

**Status:** `partial` — the exception becomes a nested data class (in
struct form). It explicitly does **not** extend `RuntimeException`; the
crate justifies this with `serialVersionUID`.

### 7.4.2 Interface forward declaration: no Java mapping

**Spec:** §7.4.2, p. 23 — "An interface forward declaration has no
mapping to the Java language."

**Repo:** —

**Tests:** —

**Status:** `open` — there is no Java forward-declaration handling; the
binding refuses an interface forward declaration without a definition in
the RPC path as E002, which is not the same thing.

---

## §7.5 Interfaces – Full

### 7.5 Embedded type/const/exception decls in the interface body

**Spec:** §7.5, p. 23 — "The embedded elements shall be mapped to a
public declaration within the scope of the Java interface."

**Repo:** —

**Tests:** —

**Status:** `open` — embedded declarations in an interface body are not
carried into the scope of the Java interface.

---

## §7.6 Value Types

### 7.6 IDL valuetype -> 2 Java classes (Abstract + concrete)

**Spec:** §7.6, p. 23-25 — "An IDL valuetype type shall be mapped
to two Java classes: A helper abstract class with the suffix
Abstract; A class with the same name [...] private members are
protected [...] Each valuetype initializer is mapped to a method
returning void."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** —

**Status:** `open` — a `valuetype` is mapped like a struct to one nested
class with `public` fields; the abstract+concrete split, `protected`
members and `void` factory methods are missing.

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

**Repo:** —

**Tests:** —

**Status:** `open` — no CCM component mapping to intermediate IDL.

---

## §7.10 Components – Homes

### 7.10 Homes -> intermediate IDL

**Spec:** §7.10, p. 25 — analogous to §7.9.

**Repo:** —

**Tests:** —

**Status:** `open` — as §7.9; no home mapping.

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

**Repo:** —

**Tests:** —

**Status:** `open` — as §7.9; no port/connector mapping.

---

## §7.13 Template Modules

### 7.13 Template-module instances -> intermediate IDL

**Spec:** §7.13, p. 25 — "Template module instances have no direct
language mapping; they shall be mapped to intermediate IDL."

**Repo:** —

**Tests:** —

**Status:** `open` — no template-module mapping.

---

## §7.14 Extended Data Types

### 7.14.1 Struct with single inheritance -> Java extends

**Spec:** §7.14.1, p. 26 — "If the IDL struct inherits from a base
IDL struct, then the Java class shall be declared to extend the
base class [...] all values constructor [...] first parameter a
non-null instance of the base struct's Java class."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** —

**Status:** `open` — the column flattens base members into the derived
class (`all_members_of`); a Java `extends` and the base-instance
constructor are not produced.

### 7.14.2 Union discriminator: int8/uint8/wchar/octet additionally allowed

**Spec:** §7.14.2, p. 26 — "This IDL4 block adds int8, uint8, wchar
and octet to the set of valid types for a discriminator."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`.

**Status:** `partial` — the discriminator is mapped generically to the
Java type of the IDL discriminator type, so also for `octet`; a test that
names an 8-bit or `octet` discriminator is missing.

### 7.14.3.1 IDL map -> java.util.Map<K,V>


**Spec:** §7.14.3.1, p. 26-27 — "An IDL map shall be mapped to a
Java generic java.util.Map instantiated with the Java equivalent
key type and value type."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_map_is_a_java_util_map_over_a_generated_entry_class`, `crates/lang-rules/src/adapter/java.rs::tests::a_map_keyed_by_a_structure_is_a_java_util_map_over_a_class_with_equals`, `crates/lang-rules/tests/java_map_class.rs::a_map_is_a_java_util_map_with_the_operations_of_table_47`

**Status:** done — a `map<K,V>` becomes a class `ZdMap_…` that extends `java.util.LinkedHashMap` over the boxed types and carries the operations of Table 47; structure, union and bit set are keys (generated `equals` and `hashCode`). Log: `.agent/REPORT.md`.

### 7.14.3.2 IDL bitset -> Java public class with bitfield accessors

**Spec:** §7.14.3.2, p. 28 — "An IDL bitset shall map to Java as a
public class with the same name. [...] Default member type: boolean
if size is 1, octet 2-8, unsigned short 9-16, unsigned long 17-32,
unsigned long long 33-64."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_bitset_is_a_class_of_one_field_per_bitfield`.

**Status:** `partial` — the bitset becomes a class with one `public` field
per bitfield. Bitfield accessors and the default-type-per-bit-size rule
are missing (fields are `int`).

### 7.14.3.3 IDL bitmask -> Java enum + java.util.BitSet

**Spec:** §7.14.3.3, p. 28-29 — "The IDL bitmask type shall map to
a Java enum and a java.util.BitSet. The Java enum name shall be the
IDL bitmask name with the Flags suffix appended. [...] If the size
exceeds @bit_bound, IndexOutOfBoundsException shall be raised."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_bitmask_takes_the_holder_its_bound_calls_for`.

**Status:** `partial` — the bitmask becomes constants in the unsigned
holder of its `@bit_bound` (`int` or `long`). A `<Name>Flags` Java `enum`
and `java.util.BitSet` are missing.

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

**Repo:** —

**Tests:** —

**Status:** `open` — user annotations are not propagated as Java
`@interface`.

### 7.16.2 Single-instance/multi-instance application

**Spec:** §7.16.2, p. 30 — "the corresponding Java element shall be
annotated with the mapped Java annotation [...] For multiple
instances, the Group-suffix annotation."

**Repo:** —

**Tests:** —

**Status:** `open` — as §7.16.1; annotations do not reach the Java.

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose Tab.7.6

**Spec:** §7.17.1 Tab.7.6, p. 32 — impact per general-purpose
annotation (`@id`, `@autoid`, `@optional`, `@position`, `@value`,
`@extensibility`/`@final`/`@mutable`/`@appendable`).

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_optional_member_is_boxed_and_a_primitive_one_too`,
`crates/lang-rules/src/adapter/java.rs::tests::an_explicit_enumerator_value_is_written_out`.

**Status:** `partial` — `@optional` boxes the member, `@value` writes the
enum value. `@id`, `@position` and the extensibility annotations are not
emitted.

### 7.17.2 Data Modeling Tab.7.7

**Spec:** §7.17.2 Tab.7.7, p. 32 — impact of `@key`,
`@must_understand`, `@default_literal`.

**Repo:** —

**Tests:** —

**Status:** `open` — no `@key`/`@must_understand` markers and no
`@default_literal` handling in the generated Java.

### 7.17.3 Units and Ranges Tab.7.8

**Spec:** §7.17.3 Tab.7.8, p. 32-33 — `@default` in the default
constructor; `@range`/`@min`/`@max` raise `IllegalArgumentException` in
the setter; `@unit` has no effect.

**Repo:** `crates/lang-rules/src/default_java.rs`.

**Tests:** —

**Status:** `partial` — `@default` reaches the `zd_def_` defaulter
(`default_java.rs::member_default`), but not the spec constructor;
`@range`/`@min`/`@max` setter checks are missing.

### 7.17.4 Data Implementation Tab.7.9

**Spec:** §7.17.4 Tab.7.9, p. 33 — `@bit_bound` (bitmask),
`@external` (boxed), `@nested` (no impact).

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_bitmask_takes_the_holder_its_bound_calls_for`.

**Status:** `partial` — `@bit_bound` chooses the bitmask's holder.
`@external` and `@nested` do not reach the Java.

### 7.17.5 Code Generation Tab.7.10: @verbatim

**Spec:** §7.17.5 Tab.7.10, p. 33 — "@verbatim copies verbatim text
to the indicated output position when the indicated language is
'*' or 'java'."

**Repo:** `crates/lang-rules/src/verbatim.rs`,
`crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::java_wraps_the_file_and_fills_the_class_body`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`.

**Status:** done — the `java` and `*` aliases are placed into the Java
output at the six spec placements.

### 7.17.6 Interfaces Tab.7.11

**Spec:** §7.17.6 Tab.7.11, p. 33 — `@service`, `@oneway`, `@ami`;
the impact is middleware-specific.

**Repo:** `crates/lang-rules/src/emit/interface_java.rs`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::java_declares_the_same_entities_as_cpp`.

**Status:** `partial` — `@service` drives the RPC interface generation.
`@oneway` and `@ami` are not evaluated.

---

## §8 IDL to Java Language Mapping Annotations

### 8.1.0 @java_mapping annotation definition

**Spec:** §8.1, p. 35 — "@annotation java_mapping { enum
NamingConvention {IDL_NAMING_CONVENTION,JAVA_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default 'Constants'; boolean promote_integer_width default FALSE;
string string_type default 'java.lang.String'; }"

**Repo:** —

**Tests:** —

**Status:** `open` — `@java_mapping` does not exist in the tree.

### 8.1.1 apply_naming_convention Tab.8.1

**Spec:** §8.1.1 Tab.8.1, p. 35-36 — the table lists 17
IDL-construct types with IDL_NAMING vs. JAVA_NAMING.

**Repo:** —

**Tests:** —

**Status:** `open` — there is no naming-scheme switch; the
transformations of §7.1.1.2.x are missing.

### 8.1.2 constants_container parameter

**Spec:** §8.1.2, p. 37 — "constants_container activates the
alternative mapping for constants defined in §7.2.3.1."

**Repo:** —

**Tests:** —

**Status:** `open` — no parameter, no `Constants` container.

### 8.1.3 promote_integer_width parameter (Tab.8.2)

**Spec:** §8.1.3 Tab.8.2, p. 37-38 — "whether IDL unsigned integers
shall be mapped to a Java primitive type of the same size (FALSE,
default) or to a bigger type (TRUE)."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — the column widens unsigned types (unconditionally,
i.e. the TRUE case), but there is no switch and therefore not the spec
default FALSE.

### 8.1.4 string_type parameter

**Spec:** §8.1.4, p. 38 — "string_type defines the Java type IDL
string and wstring types shall be mapped to. By default,
java.lang.String."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`.

**Status:** `partial` — `java.lang.String` is always used; the parameter
is missing, so the default is reached.

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1 CORBA-Specific Mappings

**Spec:** Annex A, p. 39+ — CORBA-specific adaptations.

**Repo:** —

**Tests:** —

**Status:** `open` — no IDL4-Java Annex A.1 mapping. The CORBA RMI column
`crates/corba-codegen/` implements the older IDL-to-Java 1.3 mapping
(formal/2008-01-04) for CORBA, not this annex.

---

## Audit status

4 done / 27 partial / 34 open / 22 n/a (informative) / 0 n/a (rejected).

No test run as part of this re-sync (cargo is not permitted for this
task). The citations are paths and test names in today's tree;
`python3 website/_tools/verify_coverage_items.py --check` checks the
citation and the test names.
